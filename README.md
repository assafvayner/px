# px

Currently a ping pong server built with rust, intended to eventually be a working model for creating a cluster of containers which can ensure message consistency using paxos and receive messages through a pipe from a different application on a container. Messages are passed using quic (AWS's s2n-quic library) and altogether using tokio async runtime.

## configuration

### general

Each node operates a quic server and communication between nodes is encrypted using TLS as established by quic protocol.
We operate under the assumption that communication using px is exclusively between px nodes and therefore there is no strict benefit for using public PKI and a public root certificate authority and therefore it is recommended to use a custom private root certificate with which to sign certificates. (Script to generate certificiates TBD).
Each node also must take a path to the root certificate with which to validate the other nodes, hence all nodes with which a node communicates with (not including itself) should use the same root certificate to sign their certificates. I recommend having 1 private root certificate to generate all the certificates used.

### config file

Each node takes 1 command line parameter which is a path to a json configuration file obeying the following format (comments removed)

```json
{
  "me": {
    "id": "string", // server id, should be domain name/what's in their TLS cert
    "addr": "ip:port", // or otherwise local address to bind to
    // info needed to configure secure comms
    "tls_config_info": {
      // files under paths should be in pem format
      "cert_path": "string to certificate",
      "key_path": "string path to private key",
      "ca_cert_path": "string path to root ca cert with which to validate other nodes"
    }
  },
  // amount of time to wait between reconnection attempts, base for exponential backoff TBD, in milliseconds
  "retry_delay": 1000,
  // other nodes to communicate with
  "servers": [
    // server information objects containing 2 fields:
    {
      "addr": "<address>", // e.g. "ip:port" or "hostname"
      "server_name": "string" // server name with which their TLS certificate is signed, should be domain name.
    }
  ]
}
```

## Connection management

For each pair of nodes there are 2 quic connections with 1 stream each.
At every node there is a quic server accepting listening streams and messages are received on those streams.
Each node also has a set of client connections which each open 1 send stream and they use those streams to send messages.
The ConnectionManager module and struct singleton handles all the client connections and send streams.

## Messages

Messages on stream are json serialized Message structs delimited with a `\r`.
MessageParser provides an interface to append data and then iterate over parsed messages to let an Application handle them.
The root cause for this necessary interface is that quic receiving stream interface is not guarenteed to provide 1 "`Message`" at a time, since it is simply a byte stream.
For this reason we need some method of delimiting messages and handling potential buffering.
This interface does not guarentee anything about message order or that messages would not fail to be parsed.

## Timers

To provide a simple method of running some async code after a delay, the timer module provides a `timer` function that takes some future and executes it only after a specified duration.

## runX.sh

Provided scripts to run X number of px nodes with pre-configured config files in the config directory.
To stop them running kill them with `Ctrl+D` or the command `killall px` (although then you'll need to separately kill the `runX.sh` bash process).

### Certificates not provided
The config files have the names of some of the certificate files that I use that I generated for myself, I cannot publish them on github and will provide a method to generate those certificates in the future.
