package cli

type usageError struct{ err error }

func (e *usageError) Error() string { return e.err.Error() }
func (e *usageError) Unwrap() error { return e.err }

type gitExitError struct {
	code int
	err  error
}

func (e *gitExitError) Error() string { return e.err.Error() }

func (e *gitExitError) Unwrap() error { return e.err }
