# warp-demo
debug: fmt
	cargo build

release: fmt
	RUSTFLAGS="-C target-cpu=native" cargo build --release

RUN_CMD = target/release/warp-demo

run:
	$(RUN_CMD)

silent-run:
	$(RUN_CMD) >/dev/null 2>&1

debug: fmt
	cargo build

fmt:
	cargo fmt

bench:
	RUSTFLAGS="-C target-cpu=native" cargo bench

REQUEST_URL = http://localhost:3030/math/4
REQUEST_HEADER_GOOD = -H 'div-by: 2'
REQUEST_HEADER_BAD = -H 'div-by: 0'
GOOD_REQUEST = $(REQUEST_URL) $(REQUEST_HEADER_GOOD)
BAD_REQUEST = $(REQUEST_URL) $(REQUEST_HEADER_BAD)
REQ_ID_HEADER = -H 'x-request-id: my-custom-external-id-here'
WRK_SETTINGS =  -t 4 -c 10 -R 200 -d 30 -L

requests:
	echo 'Request test ...'
	echo '=== success; internal ID'
	curl -i $(GOOD_REQUEST)
	echo '=== success; external ID'
	curl -i $(GOOD_REQUEST) $(REQ_ID_HEADER)
	echo '=== failure; internal ID'
	curl -i $(BAD_REQUEST)
	echo '=== failure; external ID'
	curl -i $(BAD_REQUEST) $(REQ_ID_HEADER)

# wrk2 for mac: https://github.com/giltene/wrk2/wiki/Installing-wrk2-on-Mac
perftest: requests
	echo 'Performance test ...'
	echo '=== success; internal ID'
	wrk $(WRK_SETTINGS) $(GOOD_REQUEST)
	echo '=== success; external ID'
	wrk $(WRK_SETTINGS) $(GOOD_REQUEST) $(REQ_ID_HEADER)
	echo '=== failure; internal ID'
	wrk $(WRK_SETTINGS) $(BAD_REQUEST)
	echo '=== failure; external ID'
	wrk $(WRK_SETTINGS) $(BAD_REQUEST) $(REQ_ID_HEADER)

# cargo install cargo-readme
readme:
	cargo readme > README.md
