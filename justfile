default:
    @just --list

test:
    QT_TYPES_JSON=qt_types_6.3.2.json cargo test

test-filter PATTERN:
    cargo test {{PATTERN}}

snapshots-update:
    @find test_resources -name "snapshot.ron" -delete
    @find test_resources/snapshots -name "*.ron" -delete 2>/dev/null || true
    QT_TYPES_JSON=qt_types_6.3.2.json cargo test

snapshot-reset DIR:
    @rm -f test_resources/{{DIR}}/snapshot.ron
    QT_TYPES_JSON=qt_types_6.3.2.json cargo test {{DIR}}

snapshots-list:
    @find test_resources -name "*.ron" | sort

snapshot-show DIR:
    @cat test_resources/{{DIR}}/snapshot.ron

build:
    cargo build

# Build with embedded Qt types (pre-generated JSONs required)
build-with-qt QT_JSONS:
    INCLUDED_QT_TYPES="{{QT_JSONS}}" cargo build

build-release:
    cargo build --release

check:
    cargo check

lint:
    cargo clippy -- -D warnings

fmt:
    cargo fmt

fmt-check:
    cargo fmt -- --check

clean:
    cargo clean

clean-all: clean
    @find test_resources -name "*.ron" -delete

fix:
    cargo +nightly fmt
    cargo clippy --fix --allow-dirty --allow-staged --all-targets
    cargo +nightly fmt

buildall:
    cargo build
    cargo build --release

# Builds
build_with_qt:
    INCLUDED_QT_TYPES="qt_types_6.3.2.json,qt_types_6.8.3.json,qt_types_6.11.0.json" cargo build

# Generate Qt types JSON from an installed Qt (run once per Qt version)
gen-qt VER:
    cargo build
    target/debug/qml_static_analyzer generate-qt-types --qt-path $HOME/Qt/{{VER}}/gcc_64

ai VERSION="6.3.2":
    just build_with_qt
    target/debug/qml_static_analyzer check --path test_project_ai --config test_project_ai/config.toml --builtin-qt-version {{VERSION}} 2>&1 | tee snap_ai.txt

proj VERSION="6.3.2":
    just build_with_qt
    target/debug/qml_static_analyzer check --path test_project --builtin-qt-version {{VERSION}} 2>&1 | tee snap_proj.txt

# Like proj but with --complex: show full element hierarchy in error paths
projc VERSION="6.3.2":
    just build_with_qt
    target/debug/qml_static_analyzer check --path test_project --builtin-qt-version {{VERSION}} --complex 2>&1 | tee snap_proj.txt

# Run checker on gui/src using a pre-generated JSON
testp VERSION="6.3.2":
    just build_with_qt
    target/debug/qml_static_analyzer check --path test --builtin-qt-version {{VERSION}} 2>&1 | tee snap_test.txt

install:
    INCLUDED_QT_TYPES="qt_types_6.3.2.json,qt_types_6.8.3.json,qt_types_6.11.0.json" cargo install --path . --locked

list-builtins:
    cargo run -- list-builtins


zigbuild:
    rm qml_static_analyzer || true
    INCLUDED_QT_TYPES="qt_types_6.3.2.json,qt_types_6.8.3.json,qt_types_6.11.0.json" cargo zigbuild --release --target x86_64-unknown-linux-musl
    cp target/x86_64-unknown-linux-musl/release/qml_static_analyzer .

### GUI-related helper
update_config:
    python3 tools/gui/generate_maincontent_children_gui.py --src gui --config ./config_gui

test_inside:
    cp config_gui gui/config_gui
    cd gui; sed -i 's|gui/||g' config_gui
    cd gui; qml_static_analyzer check --path src --config config_gui --builtin-qt-version 6.3.2

# Run checker on gui/src using a pre-generated JSON
gui VERSION="6.3.2":
    just build_with_qt
    cd gui;../target/debug/qml_static_analyzer check --path src --config config_gui --builtin-qt-version {{VERSION}} 2>&1 | tee snap_gui.txt

# Like gui but with --complex: show full element hierarchy in error paths
guic VERSION="6.3.2":
    just build_with_qt
    cd gui;../target/debug/qml_static_analyzer check --path src --config config_gui --builtin-qt-version {{VERSION}} --complex 2>&1 | tee snap_gui.txt

custom PATH VERSION="6.3.2":
    just build_with_qt
    target/debug/qml_static_analyzer check --path {{PATH}} --builtin-qt-version {{VERSION}} 2>&1 | tee snap_custom.txt

syncgui:
    rm -rf gui
    cp -r ~/Projekty/ap-600/gui .