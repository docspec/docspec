# DocSpec Manifesto

We are memory extremists. We do not apologize for this. We do not compromise.

## The River and the Lake

Software today treats memory like an infinite resource. A single web page consumes more RAM than an entire operating system from two decades ago. Document conversion libraries load entire files into memory before processing a single byte. A 100 MB document becomes 500 MB in heap allocation. This is not engineering. This is negligence.

There is no memory availability crisis. There is a *use* crisis. Your laptop has 16 GB. Your phone has 8 GB. But software squanders it with lazy abstractions, unnecessary buffering, and the assumption that garbage collection will clean up the mess.

Somewhere along the way, the industry decided that hardware would always outpace software's appetite. That Moore's Law would forever excuse Moore's waste. This bet has failed. Devices are plateauing. Edge computing is growing. Embedded systems are everywhere. And the software that runs on them still behaves as if it owns the machine.

We believe something different. Software should earn every byte it uses. Not borrow. Not assume. Earn. Justify. Document. Every allocation must have a purpose. Every buffer must have a boundary. This is not austerity. This is respect, for the hardware, for the user, for the craft.

A document is not a tree sitting in memory. A document is a river.

It has a source and a mouth. Between them, the water moves. It does not stop. It does not pool. It flows. You do not drain a river into a lake to study it. You stand at the bank and observe what passes.

DocSpec treats documents this way. A reader stands at the source, parsing bytes into a stream of structured events, one at a time, in document order. A writer stands at the mouth, consuming those events and emitting bytes in the target format. Between them, nothing accumulates. Everything moves.

This is not a performance optimization bolted onto a conventional design. This is the architecture. The streaming model is not what DocSpec *does*. It is what DocSpec *is*. Every design decision flows from this constraint. The decoupled readers and writers exist because we need to mix and match without buffering intermediates. The strict error handling exists because we cannot afford to crash and restart with half a document still in a pipe.

The result is a library that converts between any supported format combination on a $5 microcontroller with 512 KB of RAM. Not as a party trick. As proof. Proof that streaming-first, memory-conscious design works at the extreme edge. Proof that constraints do not limit, they liberate. When you cannot buffer the world, you must design for flow. When you cannot allocate freely, you must think clearly.

The same code runs unchanged on servers with gigabytes of memory, in browsers through WebAssembly, on desktops and phones and devices that do not exist yet. The architecture that works under extreme constraint works everywhere. That is not a coincidence. That is the point.

## The Language of Proof

We chose Rust because it shares our values.

Memory layout, allocation, lifetimes, all explicit, all controlled, all visible. No garbage collector making decisions behind our backs. The borrow checker enforces at compile time what other languages discover at runtime through crashes and memory leaks. Every byte accounted for. Every lifetime proven correct. Before the program runs.

Rust is not our religion. It is our tool. The type system is our safety net. The ownership model is our guide. The lack of garbage collection is not a limitation, it is a liberation. We decide when memory is allocated, when it is freed, and how it is laid out. Other languages ask you to trust the runtime. Rust asks you to prove your intent. We prefer proof.

## What We Owe to Each Other

Our library runs inside other people's applications. We are guests in their process. We must be good citizens, frugal, predictable, respectful. Every byte we allocate is a debt against someone else's system. We profile obsessively. We measure. We count allocations per document. We earn our footprint, because the alternative is stealing someone else's.

Data flows event by event. Nothing accumulates. Readers produce events. Writers consume them. Any reader connects to any writer. The event model is universal. An embedded image never sits fully in memory, it streams through, chunk by chunk, from source to sink. The only acceptable reason to buffer is when the format itself demands random access, and even then, we buffer the minimum. Everything else flows.

On corruption or error, we surface it immediately. No partial output. No silent truncation. A failed conversion is better than a wrong conversion. Wrong conversions propagate, corrupt databases, and erode the trust that takes years to build. Partial success is failure in disguise. Every operation that can fail returns a Result. Every error propagates with context. Every panic is a bug we must fix.

The workspace forbids unsafe code entirely. No exceptions. We do not suppress warnings. If the compiler is unhappy, we fix the code. The linter is not an annoyance. It is a collaborator. These are not rules we follow because someone told us to. They are constraints we chose because they make the code better than we could make it alone.

Good architecture is fast architecture. Not because of compiler flags or micro-optimizations, but because the design respects the machine. Cache-friendly data structures. Zero-copy where possible. Allocation pools. Streaming instead of buffering. These are architectural decisions, not afterthoughts. Performance is not a feature you bolt on at the end. It is a property you maintain from the start, or you never have it at all.

Every dependency is a bet on someone else's priorities, someone else's schedule, someone else's security practices. They update. They break. They carry transitive weight. Small code you wrote and understand is better than large code you imported and hope works. Every dependency must earn its place with written justification. If we can build it ourselves within reason, we do.

Documentation is not separate from code. It is code. Every public item is documented. Every function has a doc comment. Every module has a purpose statement. Undocumented code does not compile. Code tells you what it does. Documentation tells you why. "I will document it later" is a promise that is never kept. We document now, or we do not ship.

## The Diseases We Refuse to Catch

We have watched projects die from the same diseases, and we refuse to catch them.

The performance trap: "It works on my machine." "The tests pass." "It is fast enough for now." These phrases sound reasonable. They are not. They are the slow death of quality, the quiet acceptance that things will get worse, the assumption that someone else will fix it later. No one fixes it later. We measure. We track. We regress. Every change is checked for impact.

The abstraction trap: treating memory like an infinite scroll, trusting the runtime to clean up, buffering because it is easier than streaming. Easier for the developer. Brutal for the user. We choose the harder path, not because we enjoy difficulty, but because the harder path is the honest one. Streaming is harder to write. It is also the only way to process a document larger than available memory. We do not choose between easy and correct. We choose correct.

The debt trap: skipping tests, relaxing coverage, suppressing warnings instead of fixing code, merging without review, documenting later. Every shortcut is a crack in the foundation. Projects that tolerate cracks eventually collapse under their own weight. We are rigorous not because we distrust contributors, but because we respect the codebase enough to keep it whole.

## The Proof Is in the Running

DocSpec converts documents on microcontrollers. Any reader to any writer. Streaming. Event-based. Memory-conscious. This is not marketing. This is validation.

The constraints forced us to be good. The constraints kept us honest. And the same discipline that lets us run on 512 KB of RAM also makes us fast, portable, and reliable on machines with a thousand times more. Efficiency does not trade off against capability. Efficiency *is* capability.

## The World We Are Building

We build DocSpec for developers integrating document conversion into their applications. But the vision extends beyond our library.

Imagine software that does not force you to upgrade your hardware. Software that runs on the phone you already own, the laptop you bought five years ago, the device that will never see another firmware update. Software that does not assume a fast connection to someone else's cloud. Software that works *here*, on *this* machine, with *these* resources, and works well.

This is sovereignty. When your tools do not demand the latest hardware, you are free to choose when to upgrade, or not to. When your software does not phone home, you own your workflow. When a library respects the memory it is given, it collaborates with every other piece of software on the system instead of competing with them.

Efficient software is also collaborative software. A library that takes only what it needs leaves room for everything else. A process that streams instead of buffering lets the operating system breathe. A tool that fails clearly and immediately lets the developer fix the problem instead of chasing ghosts. This is what it means to be a good citizen of someone else's system.

We believe the industry can build this way. Not every project will run on a microcontroller, nor should they. But every project can ask the question: *do we need this allocation?* Every project can measure its footprint. Every project can stream instead of buffer, fail fast instead of fail silently, document instead of defer.

The future of software is not in bigger machines. It is in better code. Code that respects resources. Code that respects users. Code that gives people sovereignty over their own hardware.

## For Those Who Build With Us

If you are here to contribute: welcome. Know what you are joining.

We are not relaxed. We are not permissive. We have standards, and we enforce them, not as gatekeeping, but as stewardship. Your code will be reviewed carefully. Your tests must pass. Your documentation must be complete. This is how we keep the project alive. This is how we keep the project worth contributing to.

If this sounds demanding, consider the alternative. Projects without standards accumulate debt, become impossible to maintain, and die. We want DocSpec to live. We want it to thrive. Discipline is how.

We want your contributions. We want your ideas. We want your energy. We want them within the boundaries that make them lasting.

See [CONTRIBUTING.md](CONTRIBUTING.md) for the workflow and [CODING_STANDARDS.md](CODING_STANDARDS.md) for the technical details.

## For Those Who Build On Us

If you use DocSpec: thank you. You are why we exist.

You do not need to know the manifesto to use the library. But we hope you feel the difference. The conversions that complete without devouring your RAM. The errors that tell you exactly what went wrong. The documentation that actually answers your questions. The dependency that earns its place in your project the way we demand every dependency earns its place in ours.

This is what software should be. Focused. Efficient. Honest. We respect your resources. We respect your time. We respect your trust.

## The Invitation

We are memory extremists. We convert documents using less memory than most applications use to display a settings dialog. We do it safely. We do it correctly. We do it with discipline.

But this manifesto is not really about memory. It is about care. Care for the machine that runs the code. Care for the developer who reads it. Care for the user who depends on it. Care for the craft of building software that lasts.

Software should earn every byte it uses. We write software that does. We invite you to write it with us.
