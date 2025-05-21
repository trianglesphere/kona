FROM ubuntu

COPY dist/kona /usr/local/bin

EXPOSE X/udp X
ENV RUST_LOG=debug
ENTRYPOINT ["/usr/local/bin/kona"]