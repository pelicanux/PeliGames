#!/bin/sh
printf '%s\n' "$$" > pid
while IFS= read -r line; do
    rest=${line#*'"id":'}
    request_id=${rest%%,*}
    case "$line" in
        *'"method":"initialize"'*)
            case '__MODE__' in
                silent) sleep 30; continue;;
                identity) identity=incorrect; api=1;;
                api) identity=__ID__; api=2;;
                *) identity=__ID__; api=1;;
            esac
            printf '{"jsonrpc":"2.0","id":%s,"result":{"host_api":%s,"addon_id":"%s","addon_version":"__VERSION__"}}\n' "$request_id" "$api" "$identity";;
        *'"method":"shutdown"'*)
            if [ '__MODE__' = ignore-stop ]; then sleep 30; continue; fi
            if [ '__MODE__' = background ]; then sleep 1; : > finalized; fi
            : > graceful
            printf '{"jsonrpc":"2.0","id":%s,"result":null}\n' "$request_id"
            exit 0;;
        *'"method":"remote-error"'*) printf '{"jsonrpc":"2.0","id":%s,"error":{"code":-32601,"message":"unsupported"}}\n' "$request_id";;
        *'"method":"bad"'*) printf '{"jsonrpc":"2.0","id":%s,"result":null}\n' "$((request_id + 1))";;
        *'"method":"oversized"'*) head -c 1048600 /dev/zero | tr '\000' x;;
        *'"method":"partial"'*) printf '{"jsonrpc":"2.0"'; sleep 30;;
        *'"method":"crash"'*) exit 7;;
        *'"method":"hang"'*) : > called; sleep 30;;
        *'"method":"delay"'*) : > called; sleep 0.1; printf '{"jsonrpc":"2.0","id":%s,"result":null}\n' "$request_id";;
        *'"method":"stderr"'*) head -c 262144 /dev/zero >&2; printf '{"jsonrpc":"2.0","id":%s,"result":null}\n' "$request_id";;
        *'"method":"spawn-child"'*) sleep 30 & child=$!; printf '{"jsonrpc":"2.0","id":%s,"result":{"pid":%s}}\n' "$request_id" "$child";;
        *'"method":"environment"'*) printf '{"jsonrpc":"2.0","id":%s,"result":{"secret":"%s"}}\n' "$request_id" "$PELIGAMES_ADDON_TEST_SECRET";;
        *) printf '{"jsonrpc":"2.0","id":%s,"result":{"pid":%s,"version":"__VERSION__"}}\n' "$request_id" "$$";;
    esac
done
