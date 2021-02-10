#!/bin/bash
#set -x
#set -e
#set -v
# don't run when not on first local console
if [[ "$(tty)" != /dev/tty1 ]]; then
	exit 0
fi

myfolder="/media/picframe/"
# check for local nfs mount and retry if it's not reachable
while [ true ]
do
	if [[ -e "$myfolder/pc-picframe" ]]; then
		echo "Constructing image list. This may take a while..."
		find -L $myfolder -name "*.jpg" -type f -print |shuf  > /home/pi/pic1.list
				/home/pi/rahmen -o /dev/fb0 --buffer_max_size 5120000 --config /home/pi/rahmen.toml /home/pi/pic1.list
	else
		echo  "$myfolder/pc-picframe not found. Retrying."
		sleep 60
	fi
done
exit 0
