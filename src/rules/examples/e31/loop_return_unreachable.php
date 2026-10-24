<?php

function loop_return_unreachable(): void
{
    while (true) {
        return;
    }

    echo "unreachable";
}
