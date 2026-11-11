//! The page that asks for a picker command's argument. Accepting a command's
//! row in the commands picker pushes it: a one-line prompt that shows the
//! command's name and takes the argument as typed. Enter runs the command
//! with it. What the command returns goes on top, so Esc from there comes
//! back to the argument, to change it and run again, and Esc here goes back
//! to the commands.
use crate::config::PickerCommand;
use crate::picker::{Outcome, PickerHost, PickerRequest, PickerView};
use nfm_egui::shared::SharedPicker;
use nfm_egui::{Appearance, Placement, Prompt, PromptConfig, PromptEvent, egui};

/// What accepting `command`'s row does: ask for its argument on a page of
/// its own.
pub(crate) fn start(command: &PickerCommand) -> Outcome {
    Outcome::Push(Box::new(Request {
        command: command.clone(),
    }))
}

struct Request {
    command: PickerCommand,
}
impl PickerRequest for Request {
    fn build(self: Box<Self>, _: &PickerHost<'_>) -> Result<Box<dyn PickerView>, String> {
        Ok(Box::new(view(self.command)))
    }
}

fn view(command: PickerCommand) -> View {
    let argument = &command.argument;
    let placeholder = if argument.description.is_empty() {
        format!("<{}>", argument.name)
    } else {
        format!("<{}>  {}", argument.name, argument.description)
    };
    let prompt = Prompt::new(
        "swm-command-argument",
        PromptConfig {
            prompt: format!("{}:", command.name),
            placeholder,
            initial_text: String::new(),
        },
    );
    View {
        prompt,
        command,
        failed: None,
    }
}

struct View {
    prompt: Prompt,
    command: PickerCommand,
    /// The argument the error showing is about; it goes when the argument
    /// changes.
    failed: Option<String>,
}
impl View {
    fn fail(&mut self, error: String) {
        self.failed = Some(self.prompt.text().to_owned());
        self.prompt.set_error(Some(error));
    }
}
impl PickerView for View {
    fn picker(&self) -> Option<&SharedPicker> {
        None
    }
    fn has_header(&self) -> bool {
        false
    }
    fn height(&self, ctx: &egui::Context, appearance: &Appearance) -> Option<f32> {
        Some(self.prompt.height(ctx, appearance))
    }
    fn show(
        &mut self,
        ctx: &egui::Context,
        placement: Placement,
        appearance: &Appearance,
        _: bool,
    ) -> Option<Outcome> {
        let event = self.prompt.show(ctx, placement, appearance, false).event;
        if let Some(argument) = &self.failed
            && argument != self.prompt.text()
        {
            self.failed = None;
            self.prompt.set_error(None);
            ctx.request_repaint();
        }
        match event {
            Some(PromptEvent::Submitted(argument)) => {
                if self.command.argument.required && argument.is_empty() {
                    self.fail(format!("{} is required", self.command.argument.name));
                    return None;
                }
                match (self.command.run)(&argument) {
                    Ok(request) => Some(Outcome::Push(request)),
                    Err(error) => {
                        self.fail(error);
                        None
                    }
                }
            }
            Some(PromptEvent::Cancelled) => Some(Outcome::Back),
            None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CommandArgument;
    use std::sync::Arc;

    fn command(
        run: impl Fn(&str) -> Result<Box<dyn PickerRequest>, String> + Send + Sync + 'static,
    ) -> PickerCommand {
        PickerCommand {
            name: "Echo".into(),
            description: "Says its argument".into(),
            argument: CommandArgument {
                name: "text".into(),
                description: "What to say".into(),
                required: true,
            },
            run: Arc::new(run),
        }
    }
    fn key(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }
    /// Runs a frame of `view` with `events`: its outcome, and whether the
    /// frame drew `needle`.
    fn frame(
        ctx: &egui::Context,
        view: &mut View,
        events: Vec<egui::Event>,
        needle: &str,
    ) -> (Option<Outcome>, bool) {
        let bounds = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        let mut outcome = None;
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(bounds),
                events,
                ..Default::default()
            },
            |ui| {
                outcome = view.show(
                    ui.ctx(),
                    Placement::new(bounds),
                    &Appearance::default(),
                    false,
                )
            },
        );
        let drew = output.shapes.iter().any(|shape| {
            matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text.contains(needle))
        });
        output.drop_without_applying_deltas();
        (outcome, drew)
    }

    #[test]
    fn enter_runs_the_command_with_the_argument_as_typed() {
        let ran = Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen = ran.clone();
        let command = command(move |text| {
            seen.lock().unwrap().push(text.to_owned());
            Ok(Box::new(Request {
                command: self::command(|_| Err("unused".into())),
            }) as Box<dyn PickerRequest>)
        });
        assert!(matches!(start(&command), Outcome::Push(_)));
        let ctx = egui::Context::default();
        let mut view = view(command);
        frame(&ctx, &mut view, Vec::new(), "");
        let (_, prompted) = frame(&ctx, &mut view, Vec::new(), "Echo:");
        assert!(prompted);
        assert!(frame(&ctx, &mut view, Vec::new(), "<text>  What to say").1);

        // A required argument left empty does not run the command.
        let (outcome, _) = frame(&ctx, &mut view, vec![key(egui::Key::Enter)], "");
        assert!(outcome.is_none());
        assert!(frame(&ctx, &mut view, Vec::new(), "text is required").1);
        assert!(ran.lock().unwrap().is_empty());

        let typed = egui::Event::Text(" a \"b\"".into());
        let (outcome, _) = frame(&ctx, &mut view, vec![typed, key(egui::Key::Enter)], "");
        assert!(matches!(outcome, Some(Outcome::Push(_))));
        assert_eq!(*ran.lock().unwrap(), [" a \"b\""]);
        // The page stays as it was, for coming back to.
        assert_eq!(view.prompt.text(), " a \"b\"");
        assert!(!frame(&ctx, &mut view, Vec::new(), "text is required").1);

        let (outcome, _) = frame(&ctx, &mut view, vec![key(egui::Key::Escape)], "");
        assert!(matches!(outcome, Some(Outcome::Back)));
    }

    #[test]
    fn why_a_command_did_not_run_shows_until_the_argument_changes() {
        let ctx = egui::Context::default();
        let command = command(|text| Err(format!("cannot say {text}")));
        let mut view = view(command);
        frame(&ctx, &mut view, Vec::new(), "");
        let typed = egui::Event::Text("hi".into());
        let (outcome, _) = frame(&ctx, &mut view, vec![typed, key(egui::Key::Enter)], "");
        assert!(outcome.is_none());
        assert!(frame(&ctx, &mut view, Vec::new(), "cannot say hi").1);
        frame(&ctx, &mut view, vec![egui::Event::Text("!".into())], "");
        assert!(!frame(&ctx, &mut view, Vec::new(), "cannot say hi").1);
        assert_eq!(view.prompt.text(), "hi!");
    }
}
