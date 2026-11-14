#[test]
fn proposed_host_contract_is_valid_wit_but_is_not_a_current_sdk_import() {
    let mut resolve = wit_parser::Resolve::default();
    let package = resolve
        .push_str("proposal.wit", include_str!("../../host-api/proposal.wit"))
        .unwrap();
    assert_eq!(resolve.packages[package].name.namespace, "sorputi");
}

#[test]
fn proposed_project_files_interface_is_valid_wit() {
    let mut resolve = wit_parser::Resolve::default();
    let package = resolve
        .push_str(
            "project-files.wit",
            include_str!("../../host-api/project-files.wit"),
        )
        .unwrap();
    assert_eq!(resolve.packages[package].name.namespace, "sorputi");
}
