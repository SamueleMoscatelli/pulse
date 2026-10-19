# Testing

Unit tests live next to the code they cover and must not touch the network, the clock or
the file system outside a temporary directory; use the provided fakes for each of them so
the suite stays fast and deterministic on every machine.

Integration tests live under the tests folder and exercise the public command line end to
end, with real files in a scratch workspace; keep one scenario per test and name the test
after the behavior it checks, not after the function it calls.
