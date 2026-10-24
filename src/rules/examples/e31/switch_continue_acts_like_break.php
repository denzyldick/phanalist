<?php

function switch_continue_acts_like_break(int $value): void
{
    while ($value > 0) {
        switch ($value) {
            default:
                continue;
        }

        1 / 0;
    }
}
