default: build

build:
    @PKG_CONFIG_PATH=$HOMEBREW_PREFIX/opt/libarchive/lib/pkgconfig cargo build
