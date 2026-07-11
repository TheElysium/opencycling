// One-shot generator for the typed frontend bindings:
//
//     cargo run --bin export_bindings
//
// Rewrites src/lib/bindings.ts from the command/type graph declared in lib.rs.
// The same export also runs automatically on every debug startup (`pnpm tauri dev`),
// so this bin exists for regenerating without launching the app (e.g. in CI or
// before running `pnpm check` standalone).
//
// This is a bin target (not a #[test]) on purpose: see the doc comment on
// `export_typescript_bindings` in lib.rs.

fn main() {
    opencycling_lib::export_typescript_bindings();
}
