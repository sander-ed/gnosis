# gnosis

A small Go CLI proof of concept. The longer-term goal is to manage OKF knowledge
packages in Git repositories: download packages, propose changes, and upload
updates, with package metadata described in YAML or JSON. Those features are not
implemented yet.

## Usage

With Go 1.27.1 or later installed, build the CLI:

```sh
go build -o gnosis .
./gnosis --input "hello world"
./gnosis -i "hello world"
```

Both commands print `hello world` followed by a newline. Text is preserved as
provided; omitting the input flag or passing an empty string prints a blank line.
Use `./gnosis --help` to see the available flags.

To make `gnosis` available without the `./` prefix, run `go install .` and ensure
your Go binary directory (`go env GOBIN`, or `$(go env GOPATH)/bin` when `GOBIN` is
empty) is on your `PATH`:

```sh
gnosis -i "hello world"
```

You can also run directly from source:

```sh
go run . --input "hello world"
```

## Tests

```sh
go test ./...
```