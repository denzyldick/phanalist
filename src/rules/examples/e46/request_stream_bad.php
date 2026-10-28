<?php

function request_stream_bad(): void
{
    $body = file_get_contents('php://input');
    foreach (str_split($body) as $character) {
        echo $character;
    }
}
