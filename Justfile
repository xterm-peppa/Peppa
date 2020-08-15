test:
    RUST_LOG=debug cargo run -- 'Source Han Mono SC' 180 'aQhgj侠 ╋╋' '^-_`,.*╋💗'

test-emoji:
    RUST_LOG=debug cargo run -- 'Apple Color Emoji' 180 '💗 👌 ✅ ❌ 👍' '👨 👩 👦 👧 🀄️'

test-line:
    RUST_LOG=debug cargo run -- 'Source Code Pro for Powerline' 180 '╭─←─┬─←─╮' '╰─→─┴─→─╯'

build:
    cargo build --verbose

build-release:
    cargo build --release --verbose

lint:
    cargo clippy --all-targets --all-features --release --verbose
