<?php

final class StreamReader
{
    public function __construct(private mixed $stream)
    {
    }
}

function open_reader()
{
    $stream = stream_socket_client('tcp://example.com:80');

    $reader = new StreamReader($stream);
}
