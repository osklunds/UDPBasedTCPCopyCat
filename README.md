# UDPBasedTCPCopyCat

My own simplified TCP-like protocol that runs over UDP.

**Noteworthy features:**

- *Handles packet loss, packet duplication and out-of-order packets.*
- *Send buffer.* The sending side can send multiple packets without waiting for ACK from the receiving side, also known as pipelining, which speeds up the transmission rate. The send buffer is bounded, which means that even if the application layer sends a huge amount of data, not everything is sent at once, to prevent network overload and unnecessary dropped packets.
- *Receive buffer.* The receiving side buffers out-of-order packets until the missing packet is recevied.
- *High unit test coverage.*
- *Test case controlled timeout.* In the unit tests, the retransmission timer is controlled by the test cases. The retransmission timeout is triggered by a signal in the test case rather than having the test case sleep for the retransmission timeout time. This makes the tests faster, more stable and more trustworthy.

**Noteworthy limitations:**

- *Hardcoded retransmission timer and send buffer size.* In TCP, these are dynamically adjusted based on the network to ensure maximum speeed. With hardcoded values, it may happen that the sender sends either too slow or too fast, resulting in network overload and packet loss.
- *Slow.* While this protocol does work, it's much slower than the real deal.
- *Missing scenarios and tests.* The implementation does not handle all variants (e.g. packet loss) in the handshake and shutdown phases. It also doesn't handle intentionally incorrect packets, e.g. non-sense sequence numbers or a FIN during the three-way handshake.
- *Many TODOs.*

**Other ideas I wanna work on:**

- Alternative flow tests for client connect
- Alternative flow tests for shutdown
- Test that if timer triggers when timer no longer relevant, nothing happens
- Clean up client code
- Clean up Segment
- Enable more warnings
- More Segment tests
- wrap around seq num
- With proxy socket, for each packet, add a random delay and a random drop percentage, and a random duplication percentage. This way can test "real networks" within Rust code.
- When the demo program is run, the transmission is very slow for files ~100MB or larger. And timeouts occur. Add a test case that sends 100MB and check that no timeouts occur. Most likely such a test case won't pass directly. Some non-test code fixes are probaby needed.

