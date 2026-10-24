<?php

function loop_break_reachable(): void
{
    while (true) {
        break;
    }

    echo "reachable";
}
