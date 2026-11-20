//! The sign-in window on its own, without the app: `cargo run -p nemo-signin --example signin_window --
//! --site soundcloud --data-dir <folder>`. It is what `nemo-app --signin ...` runs. With
//! `NEMO_SIGNIN_DEBUG=1` it writes one line per cookie to stderr (names, flags and lengths, never
//! values), which is how to see why a sign-in does not finish.

fn main() {
    std::process::exit(nemo_signin::run(std::env::args_os().skip(1)));
}
