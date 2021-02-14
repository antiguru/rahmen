#!/bin/bash
# script to create a shuffled list of images to display using rahmen with this list
# could be called from .bashrc in combination with automatic login
# the folder names are examples, of course

# don't run when not on first local console
if [[ "$(tty)" != /dev/tty1 ]]; then
	exit 0
fi

myfolder="/media/picframe/"
# check for local nfs mount and retry if it's not reachable
while [ true ]
do
	if [[ -e "$myfolder/local-picframe" ]]; then
		echo "Constructing image list. This may take a while..."
		find -L "$myfolder" -name "*.jpg" -type f -print |shuf  > /home/pi/pic1.list
		/home/pi/rahmen -o /dev/fb0 --buffer_max_size 5120000 --config /home/pi/rahmen.toml /home/pi/pic1.list
	else
		echo  "$myfolder/local-picframe not found. Retrying."
		sleep 60
	fi
done
exit 0
