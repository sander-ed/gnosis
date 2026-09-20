package main

import (
	"bytes"
	"strings"
	"testing"
)

func TestRun(t *testing.T) {
	tests := []struct {
		name           string
		args           []string
		wantOutput     string
		wantCode       int
		wantError      string
		outputContains string
	}{
		{name: "long flag", args: []string{"--input", "hello world"}, wantOutput: "hello world\n"},
		{name: "short flag", args: []string{"-i", "hello world"}, wantOutput: "hello world\n"},
		{name: "equals syntax", args: []string{"--input=hello world"}, wantOutput: "hello world\n"},
		{name: "empty input", args: []string{"-i", ""}, wantOutput: "\n"},
		{name: "no arguments", outputContains: "Available Commands:"},
		{name: "preserve text", args: []string{"-i", "  knowledge\npackage  "}, wantOutput: "  knowledge\npackage  \n"},
		{name: "help", args: []string{"--help"}, outputContains: "Usage:"},
		{name: "missing long value", args: []string{"--input"}, wantCode: 2, wantError: "flag needs an argument"},
		{name: "missing short value", args: []string{"-i"}, wantCode: 2, wantError: "flag needs an argument"},
		{name: "unknown flag", args: []string{"--unknown"}, wantCode: 2, wantError: "unknown flag"},
		{name: "positional argument", args: []string{"text"}, wantCode: 2, wantError: "unknown command"},
		{name: "extra argument", args: []string{"-i", "text", "extra"}, wantCode: 2, wantError: "unknown command"},
		{name: "missing package name", args: []string{"package", "init"}, wantCode: 2, wantError: "accepts 1 arg"},
		{name: "missing owner", args: []string{"package", "init", "test"}, wantCode: 2, wantError: "--owner"},
		{
			name: "unsafe package", args: []string{"package", "init", "../escape", "--owner", "@test"},
			wantCode: 2, wantError: "invalid package name",
		},
		{
			name: "unsafe import", args: []string{"add", "../escape"},
			wantCode: 2, wantError: "invalid package name",
		},
		{
			name: "conflicting recovery flags", args: []string{"pull", "--abort", "--continue"},
			wantCode: 2, wantError: "use --continue or --abort alone",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			var stdout, stderr bytes.Buffer
			if code := run(tt.args, &stdout, &stderr); code != tt.wantCode {
				t.Errorf("exit code = %d, want %d", code, tt.wantCode)
			}
			if got := stdout.String(); tt.outputContains != "" && !strings.Contains(got, tt.outputContains) {
				t.Errorf("stdout = %q, want substring %q", got, tt.outputContains)
			} else if tt.outputContains == "" && got != tt.wantOutput {
				t.Errorf("stdout = %q, want %q", got, tt.wantOutput)
			}
			if got := stderr.String(); (tt.wantError == "" && got != "") ||
				(tt.wantError != "" && !strings.Contains(got, tt.wantError)) {
				t.Errorf("stderr = %q, want %q", got, tt.wantError)
			}
		})
	}
}
