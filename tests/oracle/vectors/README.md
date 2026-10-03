# Function-level vectors

`calls/` is the frozen JSONL snapshot of calls recorded from the JavaScript
engine before that implementation left the repository. The Rust parity tests
replay this data; the original recorder and loader hooks are no longer shipped.

The snapshot is historical test data. It cannot be regenerated from this tree.
Git retains the source history, as described by ADR 0015. Do not add, reorder,
rewrite, or delete entries in `calls/`.
