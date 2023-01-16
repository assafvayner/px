# px

Currently a ping pong server built with rust, intended to eventually be a working model for creating a cluster of containers which can ensure message consistency using paxos and receive messages through a pipe from a different application on a container. Messages are passed using quic (AWS's s2n-quic library) and altogether using tokio async runtime.
