.PHONY: build
build-DenyListUpdater:
	cargo lambda build --release --arm64
	cp -v ./target/lambda/deny_list_updater/bootstrap $(ARTIFACTS_DIR)

clean:
	rm -rf target