#!/bin/bash

function runs() {
	command="$1"

	for ((i=1;i<=5000;i++)); 
	do 
	   $command >/dev/null
	done
}

echo "Compares performance between dmesg and rmesg (assumes both are installed on your PATH)"

echo "Timing dmesg runs..."
time runs "dmesg"

echo "Timing rmesg runs..."
time runs "rmesg"

