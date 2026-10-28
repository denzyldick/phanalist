<?php

final class Cursor
{
    private int $position = 0;

    public function advance(): void
    {
        $this->position++;

        if ($this->position > 10) {
            return;
        }

        $this->advance();
    }
}
