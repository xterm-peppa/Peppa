test:
    RUST_LOG=debug cargo run -- 'Source Han Mono SC' 64 \
             'aQhgj侠 ╋╋'   \
             '^-_`,.*╋💗'   \
             '╭─←─┬─←─╮'    \
             '╰─→─┴─→─╯'    \
             'the quick brown fox jumps over'   \
             'the lazy dog'                     \
             'THE QUICK BROWN FOX JUMPS OVER'   \
             'THE LAZY DOG'

test-emoji:
    RUST_LOG=debug cargo run -- 'Apple Color Emoji' 180 '💗 👌 ✅ ❌ 👍' '👨 👩 👦 👧 🀄️'

test-line:
    RUST_LOG=debug cargo run -- 'Source Code Pro for Powerline' 180 '╭─←─┬─←─╮' '╰─→─┴─→─╯'

test-line-pingfang:
    RUST_LOG=debug cargo run -- 'PingFang SC' 180 '╭─←─┬─←─╮' '╰─→─┴─→─╯'

build:
    cargo build --verbose

build-release:
    cargo build --release --verbose

lint:
    cargo clippy --all-targets --all-features --release --verbose
