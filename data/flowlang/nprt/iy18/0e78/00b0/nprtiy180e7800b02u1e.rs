let cmd = Command::lookup(lib, ctl, cmd);
cmd.execute(params).unwrap()