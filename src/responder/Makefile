.PHONY: build
build-Responder: hosts
	cargo lambda build --release --arm64
	cp -v ./target/lambda/responder/bootstrap $(ARTIFACTS_DIR)
	cp hosts $(ARTIFACTS_DIR)

hosts:
	curl https://raw.githubusercontent.com/StevenBlack/hosts/master/hosts | sed -n -e 's/^0.0.0.0 //p' > hosts

clean:
	rm -rf target
	rm hosts