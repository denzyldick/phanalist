<?php

function server_header_bad(): void
{
    foreach (explode(',', $_SERVER['HTTP_X_VALUES']) as $value) {
        echo $value;
    }
}
