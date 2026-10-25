#!/bin/bash
# script to create a shuffled list of images to display using rahmen with this list
# could be called from .bashrc in combination with automatic login
# the folder names are examples, of course

set -o pipefail

# don't run when not on first local console
if [[ "$(tty)" != /dev/tty1 ]]; then
	exit 0
fi

myfolder="/media/picframe/"
image_list="/home/pi/pic1.list"
temporary_list=""

cleanup() {
	if [[ -n "$temporary_list" ]]; then
		rm -f -- "$temporary_list"
		temporary_list=""
	fi
}

trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

# check for local nfs mount and retry if it's not reachable
while true
do
	if [[ -e "$myfolder/local-picframe" ]]; then
		echo "Constructing image list. This may take a while..."
		# Keep the previous list until a complete, nonempty replacement is ready.
		if ! temporary_list=$(mktemp "$image_list.XXXXXX"); then
			echo "Unable to create temporary image list. Retrying." >&2
			sleep 60
			continue
		fi
		if find -L "$myfolder" -type f \( -iname "*.jpg" -o -iname "*.jpeg" -o -iname "*.png" \) -print |
			shuf > "$temporary_list" && [[ -s "$temporary_list" ]] &&
			mv -f -- "$temporary_list" "$image_list"; then
			temporary_list=""
			if ! /home/pi/rahmen -o /dev/fb0 --buffer_max_size 5120000 --config /home/pi/rahmen.toml "$image_list"; then
				echo "Slideshow exited with an error. Retrying." >&2
				sleep 60
			fi
		else
			cleanup
			echo "Unable to construct a nonempty image list. Retrying." >&2
			sleep 60
		fi
	else
		echo  "$myfolder/local-picframe not found. Retrying."
		sleep 60
	fi
done
exit 0
