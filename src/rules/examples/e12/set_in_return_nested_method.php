<?php

namespace App\Service\e12;

class SetInReturn {

    private int $counter = 0;

    public function getResponse(): string {
        return $this->return([
            'updated_counter' => ++$this->counter
        ]);
    }

    public function render($variables): string {
        return implode(', ', $variables);
    }
}