# Protocol compatibility baseline

The compatibility reference is Scouter `scouter-common` **2.21.3**, built from:

- repository: `scouter-project/scouter` (local migration source checkout)
- Git commit: `6496f5f5aeb642c3a7731a122614cd95e38684a9`
- source checkout state when recorded: `2.18.0-SNAPSHOT-94-g6496f5f5-dirty`
- reference artifact: `scouter-common-2.21.3.jar`

`tests/java/ProtocolGoldenGenerator.java` is the reproducible Java oracle for the
checked-in hexadecimal fixtures in `tests/protocol_compatibility.rs`. Regenerate
them with the exact reference JAR, then review fixture changes as protocol changes.

The initial baseline covers every Rust `Value` variant and `TextPack` in both
directions: Java bytes are decoded by Rust, then Rust re-encoding must reproduce
the same bytes. More Pack fixtures will be added incrementally until every Pack
currently exposed by the Rust enum is covered.

Decoder safety limits:

- a single blob or length-prefixed byte field is limited to 16 MiB;
- negative signed lengths are rejected as invalid data;
- truncated fields return `UnexpectedEof` rather than panicking.
