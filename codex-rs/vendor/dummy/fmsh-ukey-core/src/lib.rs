// Placeholder for the internal crate of the same name (internal GitLab
// 192.168.131.126:8089). Default builds never compile this crate: it only
// enters the graph when the `ukey` feature is enabled, and enabling it here
// means the real dependencies were not switched in.
//
// Internal full-fat builds: run `release/use-real-fm-deps.sh` first, then
// build with `--features ukey`.
compile_error!(
    "placeholder crate compiled: run `release/use-real-fm-deps.sh` to switch to the real internal dependencies before enabling the `ukey` feature"
);
