//! Image extraction from `DrawingML` (`<w:drawing>`) and VML (`<w:pict>`) subtrees.
//!
//! Both scanners walk their subtree with an explicit depth counter rather than
//! recursion, so nesting costs a counter increment instead of a stack frame.
//! Each emits an `Image` event per resolved relationship reference and drops
//! everything else in the subtree.
//!
//! These stay methods on the reader because they emit into the shared queue and,
//! on truncated input, finish the document — they are output-producing scanners,
//! not the pure property folds in [`super::props`].

use docspec_core::{Event, Result};
use quick_xml::events::BytesStart;

use super::{parse_error, read_attribute, read_decoded_attribute, DocumentReader};

#[derive(Default)]
struct DrawingScanState {
    pending_alt: Option<String>,
    current_pic_alt: Option<String>,
    pic_depth: u32,
    blip_fill_depth: u32,
    emitted_for_current_pic: bool,
}

/// State machine for parsing a `<w:pict>` VML subtree.
struct VmlScanState {
    /// Alt text stack for nested `<v:shape alt="...">` scopes.
    shape_alt_stack: Vec<Option<String>>,
}

impl DocumentReader {
    pub(super) fn parse_drawing_subtree(&mut self) -> Result<()> {
        let mut state = DrawingScanState::default();
        let mut drawing_depth: u32 = 1;

        while drawing_depth > 0 {
            let event = self.input.read_owned()?;

            match event {
                quick_xml::events::Event::Start(tag) => {
                    self.handle_drawing_child_start(&mut state, &tag);
                    drawing_depth = drawing_depth.saturating_add(1);
                }
                quick_xml::events::Event::Empty(tag) => {
                    self.handle_drawing_child_empty(&mut state, &tag);
                }
                quick_xml::events::Event::End(tag) => {
                    if tag.local_name().as_ref() == b"drawing" && drawing_depth == 1 {
                        drawing_depth = drawing_depth.saturating_sub(1);
                    } else {
                        Self::handle_drawing_child_end(&mut state, tag.local_name().as_ref());
                        drawing_depth = drawing_depth.saturating_sub(1);
                    }
                }
                quick_xml::events::Event::Eof => {
                    self.handle_eof();
                    drawing_depth = 0;
                }
                quick_xml::events::Event::Text(_)
                | quick_xml::events::Event::GeneralRef(_)
                | quick_xml::events::Event::CData(_)
                | quick_xml::events::Event::Comment(_)
                | quick_xml::events::Event::Decl(_)
                | quick_xml::events::Event::PI(_)
                | quick_xml::events::Event::DocType(_) => {}
            }
        }

        Ok(())
    }

    /// Parses a `<w:pict>` subtree and emits images for VML `<v:imagedata>` elements.
    pub(super) fn parse_pict_subtree(&mut self) -> Result<()> {
        let mut state = VmlScanState {
            shape_alt_stack: Vec::new(),
        };
        let mut pict_depth: u32 = 1;

        while pict_depth > 0 {
            let event = self.input.read_owned()?;

            match event {
                quick_xml::events::Event::Start(tag) => {
                    self.handle_pict_child_start(&mut state, &tag)?;
                    pict_depth = pict_depth.saturating_add(1);
                }
                quick_xml::events::Event::Empty(tag) => {
                    self.handle_pict_child_empty(&mut state, &tag)?;
                }
                quick_xml::events::Event::End(tag) => {
                    if tag.local_name().as_ref() == b"pict" && pict_depth == 1 {
                        pict_depth = pict_depth.saturating_sub(1);
                    } else {
                        Self::handle_pict_child_end(&mut state, tag.local_name().as_ref());
                        pict_depth = pict_depth.saturating_sub(1);
                    }
                }
                quick_xml::events::Event::Eof => {
                    return Err(parse_error(
                        "malformed document.xml: unexpected EOF inside <w:pict>".to_string(),
                    ));
                }
                quick_xml::events::Event::Text(_)
                | quick_xml::events::Event::GeneralRef(_)
                | quick_xml::events::Event::CData(_)
                | quick_xml::events::Event::Comment(_)
                | quick_xml::events::Event::Decl(_)
                | quick_xml::events::Event::PI(_)
                | quick_xml::events::Event::DocType(_) => {}
            }
        }

        Ok(())
    }

    /// Handles a start tag inside a VML `<w:pict>` subtree.
    fn handle_pict_child_start(
        &mut self,
        state: &mut VmlScanState,
        tag: &BytesStart<'_>,
    ) -> Result<()> {
        let local_name = tag.local_name();
        let local = local_name.as_ref();
        match local {
            b"shape" => {
                state
                    .shape_alt_stack
                    .push(read_decoded_attribute(tag, b"alt"));
            }
            b"imagedata" => self.emit_pict_imagedata(state, tag)?,
            _ => {}
        }
        Ok(())
    }

    /// Handles an empty tag inside a VML `<w:pict>` subtree.
    fn handle_pict_child_empty(
        &mut self,
        state: &mut VmlScanState,
        tag: &BytesStart<'_>,
    ) -> Result<()> {
        let local_name = tag.local_name();
        let local = local_name.as_ref();
        if local == b"imagedata" {
            self.emit_pict_imagedata(state, tag)?;
        }
        Ok(())
    }

    /// Handles an end tag inside a VML `<w:pict>` subtree.
    fn handle_pict_child_end(state: &mut VmlScanState, local: &[u8]) {
        if local == b"shape" {
            let _ = state.shape_alt_stack.pop();
        }
    }

    /// Emits an image event for a VML `<v:imagedata>` relationship reference.
    fn emit_pict_imagedata(
        &mut self,
        state: &mut VmlScanState,
        tag: &BytesStart<'_>,
    ) -> Result<()> {
        // TODO(vml-binData): <v:imagedata src="wordml://..."/> (inline w:binData reference) is not supported; the entry naturally degrades to "no rId found" and emits nothing.
        let image_rid = read_attribute(tag, b"r:id")
            .or_else(|| read_attribute(tag, b"r:embed"))
            .or_else(|| read_attribute(tag, b"r:link"));
        let Some(rid) = image_rid else {
            return Ok(());
        };

        let alt = read_decoded_attribute(tag, b"o:title")
            .filter(|s| !s.is_empty())
            .or_else(|| {
                state
                    .shape_alt_stack
                    .iter()
                    .rev()
                    .find_map(core::clone::Clone::clone)
            });

        // Note: if the same rId appears in both <w:drawing> and a bare <w:pict> (not inside <mc:AlternateContent>), two Image events are emitted. This is intentional — we faithfully represent what the document contains. AlternateContent-wrapped duplicates are deduped via the mc:Fallback denylist entry.
        self.emit.push(Event::Image {
            alt,
            decorative: false,
            id: None,
            source: self.package.image_source_for_rid(&rid),
            title: None,
        });

        Ok(())
    }

    fn handle_drawing_child_start(&mut self, state: &mut DrawingScanState, tag: &BytesStart<'_>) {
        let local_name = tag.local_name();
        let local = local_name.as_ref();
        match local {
            b"docPr" => {
                state.pending_alt = read_decoded_attribute(tag, b"descr");
            }
            b"pic" => {
                state.pic_depth = state.pic_depth.saturating_add(1);
                if state.pic_depth == 1 {
                    state.current_pic_alt = state.pending_alt.take();
                    state.emitted_for_current_pic = false;
                }
            }
            b"blipFill" if state.pic_depth > 0 => {
                state.blip_fill_depth = state.blip_fill_depth.saturating_add(1);
            }
            b"blip" => self.maybe_emit_drawing_blip(state, tag),
            _ => {}
        }
    }

    fn handle_drawing_child_empty(&mut self, state: &mut DrawingScanState, tag: &BytesStart<'_>) {
        let local_name = tag.local_name();
        let local = local_name.as_ref();
        match local {
            b"docPr" => {
                state.pending_alt = read_decoded_attribute(tag, b"descr");
            }
            b"blip" => self.maybe_emit_drawing_blip(state, tag),
            _ => {}
        }
    }

    fn handle_drawing_child_end(state: &mut DrawingScanState, local: &[u8]) {
        match local {
            b"blipFill" if state.blip_fill_depth > 0 => {
                state.blip_fill_depth = state.blip_fill_depth.saturating_sub(1);
            }
            b"pic" if state.pic_depth > 0 => {
                state.pic_depth = state.pic_depth.saturating_sub(1);
                if state.pic_depth == 0 {
                    state.current_pic_alt = None;
                    state.emitted_for_current_pic = false;
                }
            }
            _ => {}
        }
    }

    fn maybe_emit_drawing_blip(&mut self, state: &mut DrawingScanState, tag: &BytesStart<'_>) {
        if state.pic_depth == 0 || state.blip_fill_depth == 0 || state.emitted_for_current_pic {
            return;
        }

        let embed = read_attribute(tag, b"r:embed");
        let link = read_attribute(tag, b"r:link");
        let Some(rid) = embed.or(link) else {
            return;
        };

        state.emitted_for_current_pic = true;
        self.emit.push(Event::Image {
            alt: state.current_pic_alt.clone(),
            decorative: false,
            id: None,
            source: self.package.image_source_for_rid(&rid),
            title: None,
        });
    }
}
