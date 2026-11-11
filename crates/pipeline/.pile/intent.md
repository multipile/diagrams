Takes a diagram file to a picture through every step, and shows the steps work together on cases of every type.

What belongs here is the pipeline as one call, the `diagram` command to look at what it makes, the cases (several diagrams of every type, files that must be refused, and pairs of versions to compare), and the end-to-end tests that run them and say what each type promises. What one step does belongs to that step's crate.

Read this pile to find out what the pipeline promises from end to end, and where to add a case. Change it when a step is added or removed, a type is added, or a promise of the whole changes.
