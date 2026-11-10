# Project collaboration — work in progress

The working editor shares one `.collabtest` document. Project-wide transfer and synchronization are not available yet. A local snapshot prototype lives in `collab-helper/src/project.rs`; it is not exposed in the UI and is not sent to the server.

```text
Host selects project → preview inclusion/exclusion → bounded archive
    → authenticated encrypted transport → staging directory on guest
    → review conflicts → explicit apply → ongoing file operations + CRDT
```

The current prototype streams a gzip-compressed tar archive without loading the whole project in memory, respects project `.gitignore` rules, skips symlinks and common dependency, build, virtual environment, VCS and credential paths, limits file count and size, and refuses to overwrite an archive. A separate extraction prototype accepts only regular files in a new private staging directory and rejects traversal, duplicate paths, links, corrupt gzip and size limits; it never applies to an existing project. Limits currently default to 16 MiB per file, 512 MiB total and 50,000 files; larger individual files are omitted and counted. These filters cannot identify every secret: users need a review screen before sharing. The source and staging parent directories must not be concurrently modified by untrusted users. Empty directories are not captured. The extraction code still needs independent security review and adversarial testing.

Before this becomes a product feature:

1. Provide an inclusion/exclusion preview with editable rules, skipped-file reasons and explicit confirmation; identify all files and folders being transferred without showing secrets in logs.
2. Add a versioned, authenticated, resumable chunk protocol and bounded server storage with quotas, expiration, checksum verification, cancellation and cleanup. Do not expose the existing loopback prototype directly.
3. Extract only into a new private staging directory with strict path validation, reject traversal, links, device files, duplicates and archive bombs, and verify the final manifest before any apply.
4. Compare against guest files and open unsaved buffers. Never overwrite or delete changes silently; provide conflict preview and an explicit decision, with rollback and tests for failure midway.
5. Define permissions for creating, renaming and deleting files, CRDT document identity, ordering, offline changes, restart recovery and behavior for excluded files. Validate concurrent rename/delete/edit and reconnect.
6. Test real multi-machine operation with large synthetic projects, memory and disk quotas, network interruption, symlink attacks and malicious archives. Document supported platforms and limits.

The server is optional on client machines. For different machines, clients need a reachable server with HTTPS; starting an isolated `localhost` server on each machine does not connect them. A managed endpoint requires an operator and is not provisioned.
