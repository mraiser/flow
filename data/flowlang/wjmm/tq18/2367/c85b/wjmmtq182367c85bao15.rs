let mut out = DataObject::new();

let mut cmd = Command::new(&command)
                            .stderr(Stdio::piped())
                            .stdout(Stdio::piped())
                            .spawn()
                            .expect("failed to execute process");

let output = cmd.wait_with_output().unwrap();
let result = std::str::from_utf8(&output.stdout).unwrap();
let error = std::str::from_utf8(&output.stderr).unwrap();

out.put_str("out", result);
out.put_str("err", error);

out