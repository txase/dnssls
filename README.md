#

<p align="center">
  <img src="https://github.com/pi-hole/graphics/blob/master/Vortex/Vortex_Vertical_wordmark_lightmode.png?raw=true#gh-light-mode-only" alt="Serverless Pi-hole">
  <img src="https://github.com/pi-hole/graphics/blob/master/Vortex/Vortex_Vertical_wordmark_darkmode.png?raw=true#gh-dark-mode-only" alt="Serverless Pi-hole">
  <br>
  <strong>Global serverless ad blocking</strong>
</p>

The Serverless Pi-hole is a [DNS sinkhole](https://en.wikipedia.org/wiki/DNS_Sinkhole) that protects your devices from unwanted content without installing any client-side software.

- **Resolute**: content is blocked in _non-browser locations_, such as ad-laden mobile apps
- **Serverless**: runs in your AWS account, meaning you never have to worry about physical hardware
- **Global**: no need to run a VPN tunnel to access a DNS server when away from home
- **Secure**: DNS queries are encrypted in transit using [DNS-over-HTTPS (DoH)](https://en.wikipedia.org/wiki/DNS_over_HTTPS)
- **Cheap**: fits within the AWS free-tier for typical personal and single-family use
- **Robust**: uses modern service architectures to ensure high availability
- **Fast**: query response tm99 latency (akin to average latency) under 30ms, p99 latency (i.e. latency of worst 1% of queries) under 150ms
- **Scalable**: by default AWS limits you to 1,000 concurrent queries for safety, but Serverless Pi-hole is essentially unlimited in scalability

-----

## Instructions

TBD