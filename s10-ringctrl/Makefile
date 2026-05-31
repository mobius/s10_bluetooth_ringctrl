PREFIX ?= /usr
BINDIR = $(PREFIX)/bin
SYSCONFDIR = /etc
SYSTEMD_USER_DIR = $(PREFIX)/lib/systemd/user
UDEV_RULES_DIR = $(PREFIX)/lib/udev/rules.d

BINARY = target/release/s10_ringctrl
CONFIGS = s10-ringctrl.toml s10-ringctrl-hyprland.toml
SCRIPT = s10-herdr.sh

.PHONY: all build install uninstall clean

all: build

build:
	cargo build --release

install: build
	install -Dm755 $(BINARY) $(DESTDIR)$(BINDIR)/s10-ringctrl
	install -Dm755 $(SCRIPT) $(DESTDIR)$(BINDIR)/s10-herdr.sh
	install -Dm644 s10-ringctrl.toml $(DESTDIR)$(SYSCONFDIR)/s10-ringctrl.toml
	install -Dm644 s10-ringctrl-hyprland.toml $(DESTDIR)$(SYSCONFDIR)/s10-ringctrl-hyprland.toml
	install -Dm644 s10-ringctrl.user.service $(DESTDIR)$(SYSTEMD_USER_DIR)/s10-ringctrl.service
	install -Dm644 99-s10-ringctrl.rules $(DESTDIR)$(UDEV_RULES_DIR)/99-s10-ringctrl.rules

uninstall:
	rm -f $(DESTDIR)$(BINDIR)/s10-ringctrl
	rm -f $(DESTDIR)$(BINDIR)/s10-herdr.sh
	rm -f $(DESTDIR)$(SYSCONFDIR)/s10-ringctrl.toml
	rm -f $(DESTDIR)$(SYSCONFDIR)/s10-ringctrl-hyprland.toml
	rm -f $(DESTDIR)$(SYSTEMD_USER_DIR)/s10-ringctrl.service
	rm -f $(DESTDIR)$(UDEV_RULES_DIR)/99-s10-ringctrl.rules

clean:
	cargo clean
