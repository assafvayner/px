# run 2 ping ponging servers
# kill with Ctrl+D

cargo build
./target/debug/px config/7000.json &
./target/debug/px config/7001.json &

while true; do
    line=''

    while IFS= read -r -N 1 ch; do
        case "$ch" in
            $'\04') got_eot=1   ;&
            $'\n')  break       ;;
            *)      line="$line$ch" ;;
        esac
    done

    if (( got_eot )); then
        break
    fi
done
killall px
