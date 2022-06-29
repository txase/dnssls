.PHONY: build
build-AppleDeviceProfilePublisher:
	cargo lambda build --release --arm64
	cp -v ./target/lambda/apple_device_profile_publisher/bootstrap $(ARTIFACTS_DIR)
	cp dns.mobileconfig $(ARTIFACTS_DIR)

clean:
	rm -rf target