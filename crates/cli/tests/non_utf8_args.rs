#[cfg(unix)]
#[test]
fn invalid_utf8_arguments_are_dispatched_lossily() {
    use std::os::unix::ffi::OsStringExt;
    use std::process::Command;

    let output = Command::new(env!("CARGO_BIN_EXE_impeccino"))
        .arg(std::ffi::OsString::from_vec(vec![0xff]))
        .output()
        .expect("launch impeccino");

    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("Unknown command"));
}
