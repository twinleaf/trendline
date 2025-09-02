class IoState {
	isLogging = $state(false);
	loggingPath = $state('/Users/nguyen/Projects/GitHub/trendline/session-01.log');

	startLogging(newPath: string) {
		this.loggingPath = newPath;
		this.isLogging = true;
		console.log(`I/O Store: Logging started to ${newPath}.`);
	}
	stopLogging() {
		this.isLogging = false;
		console.log('I/O Store: Logging stopped.');
	}
    
    toggleLogging() {
        if (this.isLogging) {
            this.stopLogging();
        } else {
            this.startLogging(this.loggingPath);
        }
    }
}

export const ioState = new IoState();