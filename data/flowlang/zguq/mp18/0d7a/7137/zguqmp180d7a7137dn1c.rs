let mut globals = DataStore::globals();
if !globals.has("last_session_index") { globals.put_int("last_session_index", 0); }
let last_id = globals.get_int("last_session_index");
let mut next_id = last_id + 1;
if next_id > 65535 { next_id = 0; }
globals.put_int("last_session_index", next_id);

let s = random_non_hex_char()
  + &random_non_hex_char()
  + &random_non_hex_char()
  + &random_non_hex_char()
  + &random_non_hex_char()
  + &random_non_hex_char()
  + &format!("{:x}", time())
  + &random_non_hex_char()
  + &format!("{:x}", last_id);

s