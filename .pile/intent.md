What makes the diagram crates one pipeline: a diagram file read, painted into one document, and written from that document as a page or rendered as a picture, with one pinned toolchain, one check for every crate, and one place to find out how to run it.

The root holds what every crate is held to alike: the flake that pins the toolchain and says what is checked, the CI that runs that check on every pull request and shows the pull request every case rendered, the Cargo workspace and its settings, and the README and AGENTS.md that say how the crates fit together and how to work in them. Anything that serves only one crate belongs in that crate's pile.

Read this pile to find out what a change has to pass, what a review of it is shown, and how the steps fit together. Change it when the toolchain, the check, or the way the crates are taken moves, or when a step is added to the pipeline.
