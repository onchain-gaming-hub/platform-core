//! The committed `idls/platform_core.json` (what consumers `declare_program!` against)
//! must equal the IDL of the program being built. Fix: `anchor build && scripts/export-idl.sh`.

#[test]
fn exported_idl_matches_built_idl() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let built = std::fs::read(format!("{root}/target/idl/platform_core.json"))
        .expect("run `anchor build` first");
    let exported = std::fs::read(format!("{root}/idls/platform_core.json"))
        .expect("idls/platform_core.json missing — run scripts/export-idl.sh");
    assert!(
        built == exported,
        "idls/platform_core.json is stale — run `anchor build && scripts/export-idl.sh`"
    );
}
