package logs

import (
	"bufio"
	"fmt"
	"io"
	"os"
	"time"
)

// StreamLogs performs tailing/streaming of a log file to outChan until stopChan is closed.
func StreamLogs(logFilePath string, outChan chan<- string, stopChan <-chan struct{}) error {
	file, err := os.Open(logFilePath)
	if err != nil {
		return fmt.Errorf("open log file: %w", err)
	}
	defer file.Close()

	reader := bufio.NewReader(file)
	for {
		select {
		case <-stopChan:
			return nil
		default:
			line, err := reader.ReadString('\n')
			if err == nil {
				outChan <- line
			} else if err == io.EOF {
				time.Sleep(100 * time.Millisecond)
			} else {
				return err
			}
		}
	}
}
