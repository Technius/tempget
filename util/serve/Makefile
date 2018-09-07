FILES := file_tiny file_10m file_25m file_50m

all: $(FILES)

file_tiny:
	echo "Hello world" > $@

file_10m:
	@head -c 10M < /dev/urandom > $@

file_25m:
	@head -c 25M < /dev/urandom > $@

file_50m:
	@head -c 50M < /dev/urandom > $@

clean:
	rm -f $(FILES)

.PHONY: all clean
