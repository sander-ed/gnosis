package main

import (
	"bytes"
	"strings"
	"testing"
)

func TestRun(t *testing.T) {
	tests := []struct {
		name       string
		args       []string
		wantOutput string
		wantCode   int
		wantError  string
	}{
		{name: "long flag", args: []string{"--input", "hello world"}, wantOutput: "hello world\n"},
		{name: "short flag", args: []string{"-i", "hello world"}, wantOutput: "hello world\n"},
		{name: "equals syntax", args: []string{"--input=hello world"}, wantOutput: "hello world\n"},
		{name: "empty input", args: []string{"-i", ""}, wantOutput: "\n"},
		{name: "no arguments", wantOutput: "\n"},
		{name: "preserve text", args: []string{"-i", "  knowledge\npackage  "}, wantOutput: "  knowledge\npackage  \n"},
		{name: "help", args: []string{"--help"}, wantError: "Usage of gnosis:"},
		{name: "missing long value", args: []string{"--input"}, wantCode: 2, wantError: "flag needs an argument"},
		{name: "missing short value", args: []string{"-i"}, wantCode: 2, wantError: "flag needs an argument"},
		{name: "unknown flag", args: []string{"--unknown"}, wantCode: 2, wantError: "flag provided but not defined"},
		{name: "positional argument", args: []string{"text"}, wantCode: 2, wantError: "unexpected positional arguments"},
		{name: "extra argument", args: []string{"-i", "text", "extra"}, wantCode: 2, wantError: "unexpected positional arguments"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			var stdout, stderr bytes.Buffer
			if code := run(tt.args, &stdout, &stderr); code != tt.wantCode {
				t.Errorf("exit code = %d, want %d", code, tt.wantCode)
			}
			if got := stdout.String(); got != tt.wantOutput {
				t.Errorf("stdout = %q, want %q", got, tt.wantOutput)
			}
			if got := stderr.String(); (tt.wantError == "" && got != "") ||
				(tt.wantError != "" && !strings.Contains(got, tt.wantError)) {
				t.Errorf("stderr = %q, want %q", got, tt.wantError)
			}
		})
	}
}
