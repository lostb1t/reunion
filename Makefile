# Reunion (1994) - packaging and reverse-engineering tasks.
#
#   make            build dist/Reunion.zip for RetroArch (DOSBox Pure core)
#   make run        run the Bevy rewrite natively
#   make web        build the Bevy rewrite for the browser into dist/web
#   make serve      build and serve dist/web on http://localhost:8080
#   make test       run the Rust tests
#   make deck       build on the k3s builder pod and copy to the Steam Deck
#   make assets     extract PIC images to PNG and descramble GRWAR scripts
#   make decompile  run Ghidra headless on REUNION.PRG -> ghidra/REUNION.c
#   make clean      remove build/ and dist/

SOURCE_ZIP := Reunion_DOS_EN.zip
BUILD      := build
DIST       := dist
PACKAGE    := $(DIST)/Reunion.zip

VENV       := .venv
PYTHON     := $(VENV)/bin/python

GHIDRA     := /opt/homebrew/opt/ghidra/libexec/support/analyzeHeadless
JAVA_HOME  := /opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home

.PHONY: all assets decompile run web serve test deck clean game

WASM_OUT   := target/wasm32-unknown-unknown/wasm-release/reunion.wasm
WEB        := $(DIST)/web
PORT       := 8080

all: $(PACKAGE)

# Stage a clean copy of the original game plus the autostart batch file.
# The copy-protection crack is left out; the codes are in docs/.
$(PACKAGE): $(SOURCE_ZIP) retroarch/DOSBOX.BAT
	rm -rf $(BUILD)/package
	mkdir -p $(BUILD)/package $(DIST)
	unzip -q $(SOURCE_ZIP) -d $(BUILD)/package
	rm -rf $(BUILD)/package/CRACK $(BUILD)/package/*.zip \
		$(BUILD)/package/PATCH.COM $(BUILD)/package/RAV-TRN.COM $(BUILD)/package/JS_RETRN.EXE
	cp retroarch/DOSBOX.BAT $(BUILD)/package/
	rm -f $@
	cd $(BUILD)/package && zip -q -r -X ../../$@ .
	@echo "Built $@ - load it in RetroArch with the DOSBox Pure core."

$(PYTHON):
	python3 -m venv $(VENV)
	$(PYTHON) -m pip install -q pillow capstone

# The game's files (in the repo); the Bevy app reads them through crates/reunion/assets.
game:
	ln -sfn ../../game crates/reunion/assets

assets: $(PYTHON) game
	$(PYTHON) tools/pic2png.py game extracted/pics
	$(PYTHON) tools/grwar_descramble.py game extracted/grwar

decompile: game
	mkdir -p ghidra
	JAVA_HOME=$(JAVA_HOME) $(GHIDRA) ghidra Reunion \
		-import game/GRWAR/REUNION.PRG -overwrite \
		-scriptPath tools/ghidra_scripts -postScript ExportDecomp.java ghidra

run: game
	cargo run -p reunion

test:
	cargo test --workspace

# The original game files are served next to the page as Bevy's assets folder.
web: game
	cargo build -p reunion --profile wasm-release --target wasm32-unknown-unknown
	mkdir -p $(WEB)
	wasm-bindgen --target web --no-typescript --out-name reunion --out-dir $(WEB) $(WASM_OUT)
	cp web/index.html $(WEB)/
	rsync -a --delete --delete-excluded --exclude CRACK --exclude '*.zip' --exclude '*.COM' --exclude JS_RETRN.EXE game/ $(WEB)/assets/
	rsync -a --delete --exclude .gitkeep crates/reunion/art/ $(WEB)/art/

serve: web
	@echo "Open http://localhost:$(PORT)"
	python3 -m http.server $(PORT) --directory $(WEB)

deck: game
	./redeploy-deck.sh

clean:
	rm -rf $(BUILD) $(DIST)
