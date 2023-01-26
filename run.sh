cargo build
./target/debug/px config/5554.json &
./target/debug/px config/5555.json &
./target/debug/px config/5556.json &

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
