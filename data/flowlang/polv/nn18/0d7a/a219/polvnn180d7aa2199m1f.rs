let nonhexchars = "ghijklmnopqrstuvwxyz";
let x = rand_range(0,20) as usize;
nonhexchars[x..x+1].to_string()