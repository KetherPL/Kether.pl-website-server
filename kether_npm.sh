#!/bin/sh

# Do not change this path
PATH=/bin:/usr/bin:/sbin:/usr/sbin

# The path to the website, only change this if you have a different installation path than in the guide.
DIR=/home/rocky/Kether.pl-website-server
DAEMON="npm run"

# Change all Parameters to your needs.
# The current settings will start both the npm 'run start' and 'run server' instances
ST=start
SV=server
NAME=Kether_npm_$ST
NAME2=Kether_npm_$SV
DESC="Kether Website $DAEMON $ST/$SV"


###########################################
#                                         #
#           DON'T TOUCH THESE             #
#                                         #
###########################################

case "$1" in
	start)
		echo "Starting $DESC: $NAME / $NAME2"
		if [ -e $DIR ]; then
			cd $DIR
			screen -d -m -S $NAME $DAEMON $ST
			screen -d -m -S $NAME2 $DAEMON $SV
		else
			echo "No such directory: $DIR!"
		fi
		;;

	stop)
		if screen -ls |grep -e $NAME -e $NAME2; then
			echo -n "Stopping $DESC: $NAME / $NAME2"
			kill `screen -ls |grep $NAME |awk -F . '{print $1}'|awk '{print $1}'`
			kill `screen -ls |grep $NAME2 |awk -F . '{print $1}'|awk '{print $1}'`
			echo " ... done."
		else
			echo "Couldn't find a running $DESC"
		fi
		;;

	restart)
		if screen -ls |grep -e $NAME -e $NAME2; then
			echo -n "Stopping $DESC: $NAME / $NAME2"
			kill `screen -ls |grep $NAME |awk -F . '{print $1}'|awk '{print $1}'`
			kill `screen -ls |grep $NAME2 |awk -F . '{print $1}'|awk '{print $1}'`
			echo " ... done."
		else
			echo "Couldn't find a running $DESC"
		fi
		echo -n "Starting $DESC: $NAME / $NAME2"
		cd $DIR
		screen -d -m -S $NAME $DAEMON $ST
		screen -d -m -S $NAME2 $DAEMON $SV
		echo " ... done."
		;;

	status)
		# Check whether there's a "srcds" process
		ps aux | grep -v grep | grep Kether_npm_ > /dev/null
		CHECK=$?
		[ $CHECK -eq 0 ] && echo "NPM is UP" || echo "NPM is DOWN"
		;;

	*)
		echo "Usage: $0 {start|stop|status|restart}"
		exit 1
		;;
esac

exit 0
