FROM ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update -y && apt-get upgrade -y \
    && apt-get install -y software-properties-common gnupg2 gnupg build-essential clang gcc gcc-multilib libc-dev libc6-dev git autotools-dev cmake-data dh-elpa-helper emacsen-common file fonts-urw-base35 libarchive13 libavahi-client3 libavahi-common-data libavahi-common3 libcups2 libcurl4 libdbus-1-3 libgflags2.2 libglib2.0-0 libgmpxx4ldbl libgs9 libgs9-common libidn12 libijs-0.35 libjbig2dec0 libjsoncpp25 libmagic-mgc libmagic1 libopenjp2-7 libpaper1 librhash0 libruby3.0 libsigsegv2 libsnappy1v5 libuv1 lld-14 llvm llvm-runtime m4 poppler-data rake ruby-net-telnet ruby-rubygems ruby-webrick ruby-xmlrpc ruby3.0 rubygems-integration autoconf automake bc cmake curl dos2unix gawk ghostscript libbz2-dev libgflags-dev libgmp-dev liblz4-dev libreadline-dev libsnappy-dev libtool libzstd-dev linuxinfo lld llvm-dev ninja-build pkg-config time wget zlib1g-dev apt-transport-https

WORKDIR /root/mimalloc-bench

RUN git config --global --add safe.directory '*'

CMD ./build-bench-env.sh packages bench je tc mi mi2 hd redis rocksdb lean